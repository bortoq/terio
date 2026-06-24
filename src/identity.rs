// Identity: instance_id (ULID, permanent) + session_id (UUID, per launch)

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

const INSTANCE_FILE: &str = "instance.json";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Instance {
    pub instance_id: String,
}

#[derive(Debug, Clone)]
pub struct Identity {
    pub instance_id: String,
    pub session_id: String,
}

impl Identity {
    /// Загружает или создаёт instance_id, генерирует session_id.
    pub fn load_or_create() -> Result<Self> {
        let dir = terio_dir()?;
        std::fs::create_dir_all(&dir).context("failed to create ~/.terio")?;

        let instance_path = dir.join(INSTANCE_FILE);
        let instance_id = if instance_path.exists() {
            let content =
                std::fs::read_to_string(&instance_path).context("failed to read instance.json")?;
            let inst: Instance = serde_json::from_str(&content).context("invalid instance.json")?;
            inst.instance_id
        } else {
            let id = ulid::Ulid::new().to_string();
            let inst = Instance {
                instance_id: id.clone(),
            };
            let json = serde_json::to_string_pretty(&inst)?;
            std::fs::write(&instance_path, json).context("failed to write instance.json")?;
            id
        };

        let session_id = uuid::Uuid::new_v4().to_string();

        Ok(Self {
            instance_id,
            session_id,
        })
    }

    /// Генерирует interaction_id UUID для нового запроса.
    pub fn new_interaction_id() -> String {
        uuid::Uuid::new_v4().to_string()
    }
}

fn terio_dir() -> Result<PathBuf> {
    let home = std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .context("HOME not set")?;
    Ok(PathBuf::from(home).join(".terio"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_identity_creates_instance_file() {
        let _guard = crate::test_support::ENV_MUTEX.lock().unwrap();
        let prev_home = std::env::var("HOME").ok();
        let dir = TempDir::new().unwrap();
        std::env::set_var("HOME", dir.path());

        let identity = Identity::load_or_create().unwrap();
        assert!(!identity.instance_id.is_empty());
        assert!(!identity.session_id.is_empty());

        // Второй запуск — загружает тот же instance_id
        let identity2 = Identity::load_or_create().unwrap();
        assert_eq!(identity.instance_id, identity2.instance_id);
        // session_id разный
        assert_ne!(identity.session_id, identity2.session_id);

        if let Some(prev) = prev_home {
            std::env::set_var("HOME", prev);
        } else {
            std::env::remove_var("HOME");
        }
    }

    #[test]
    fn test_interaction_id_is_uuid() {
        let id = Identity::new_interaction_id();
        assert_eq!(id.len(), 36);
        assert_eq!(id.chars().filter(|&c| c == '-').count(), 4);
    }
}
