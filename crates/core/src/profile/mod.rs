use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// A server profile stored by the user.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Profile {
    pub id: String,
    pub label: String,
    pub host: String,
    pub port: u16,
}

impl Profile {
    pub fn new(label: &str, host: &str, port: u16) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            label: label.trim().to_string(),
            host: host.trim().to_string(),
            port,
        }
    }
}

/// Trait for profile persistence. Implementations: JsonFileStore, TauriStore.
pub trait ProfileStore: Send + Sync {
    fn list(&self) -> Result<Vec<Profile>, String>;
    fn add(&self, profile: &Profile) -> Result<(), String>;
    fn update(&self, profile: &Profile) -> Result<(), String>;
    fn delete(&self, id: &str) -> Result<(), String>;
    fn get(&self, id: &str) -> Result<Profile, String>;
}

/// JSON file-based profile store for CLI usage.
pub struct JsonFileStore {
    path: std::path::PathBuf,
}

impl JsonFileStore {
    pub fn new(path: std::path::PathBuf) -> Self {
        Self { path }
    }

    /// Default store path: ~/.config/bedrock-bridge/profiles.json
    pub fn default_path() -> std::path::PathBuf {
        let config_dir = dirs::config_dir().unwrap_or_else(|| std::path::PathBuf::from("."));
        config_dir.join("bedrock-bridge").join("profiles.json")
    }

    fn read_profiles(&self) -> Result<Vec<Profile>, String> {
        if !self.path.exists() {
            return Ok(vec![]);
        }
        let data = std::fs::read_to_string(&self.path)
            .map_err(|e| format!("read {}: {e}", self.path.display()))?;
        serde_json::from_str(&data).map_err(|e| format!("parse {}: {e}", self.path.display()))
    }

    fn write_profiles(&self, profiles: &[Profile]) -> Result<(), String> {
        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| format!("create dir: {e}"))?;
        }
        let json = serde_json::to_string_pretty(profiles).map_err(|e| format!("serialize: {e}"))?;
        std::fs::write(&self.path, json).map_err(|e| format!("write {}: {e}", self.path.display()))
    }
}

impl ProfileStore for JsonFileStore {
    fn list(&self) -> Result<Vec<Profile>, String> {
        self.read_profiles()
    }

    fn add(&self, profile: &Profile) -> Result<(), String> {
        let mut profiles = self.read_profiles()?;
        profiles.push(profile.clone());
        self.write_profiles(&profiles)
    }

    fn update(&self, profile: &Profile) -> Result<(), String> {
        let mut profiles = self.read_profiles()?;
        let existing = profiles
            .iter_mut()
            .find(|p| p.id == profile.id)
            .ok_or("profile not found")?;
        *existing = profile.clone();
        self.write_profiles(&profiles)
    }

    fn delete(&self, id: &str) -> Result<(), String> {
        let mut profiles = self.read_profiles()?;
        let before = profiles.len();
        profiles.retain(|p| p.id != id);
        if profiles.len() == before {
            return Err("profile not found".into());
        }
        self.write_profiles(&profiles)
    }

    fn get(&self, id: &str) -> Result<Profile, String> {
        let profiles = self.read_profiles()?;
        profiles
            .into_iter()
            .find(|p| p.id == id)
            .ok_or_else(|| "profile not found".into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_profile_new() {
        let p = Profile::new("  My Server  ", "  10.0.0.1  ", 19132);
        assert_eq!(p.label, "My Server");
        assert_eq!(p.host, "10.0.0.1");
        assert_eq!(p.port, 19132);
        assert!(!p.id.is_empty());
    }

    #[test]
    fn test_profile_serialization() {
        let p = Profile::new("Test", "1.2.3.4", 19132);
        let json = serde_json::to_string(&p).unwrap();
        let back: Profile = serde_json::from_str(&json).unwrap();
        assert_eq!(back.id, p.id);
        assert_eq!(back.label, "Test");
        assert_eq!(back.host, "1.2.3.4");
        assert_eq!(back.port, 19132);
    }

    #[test]
    fn test_json_file_store_crud() {
        let dir = std::env::temp_dir().join(format!("bb-test-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("profiles.json");
        let store = JsonFileStore::new(path.clone());

        // List empty
        assert!(store.list().unwrap().is_empty());

        // Add
        let p1 = Profile::new("Server A", "10.0.0.1", 19132);
        store.add(&p1).unwrap();
        assert_eq!(store.list().unwrap().len(), 1);

        // Get
        let got = store.get(&p1.id).unwrap();
        assert_eq!(got.label, "Server A");

        // Update
        let mut p1_updated = p1.clone();
        p1_updated.label = "Server B".to_string();
        store.update(&p1_updated).unwrap();
        assert_eq!(store.get(&p1.id).unwrap().label, "Server B");

        // Delete
        store.delete(&p1.id).unwrap();
        assert!(store.list().unwrap().is_empty());

        // Cleanup
        let _ = std::fs::remove_dir_all(&dir);
    }
}
