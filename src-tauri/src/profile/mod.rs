use std::sync::Mutex;
use tauri::{AppHandle, State};
use tauri_plugin_store::StoreExt;

/// Re-export core Profile
pub use bedrock_bridge_core::Profile;

/// Active profile state.
pub struct AppState {
    pub active_profile: Mutex<Option<Profile>>,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            active_profile: Mutex::new(None),
        }
    }
}

const STORE_KEY: &str = "profiles";

fn load_from_store(app: &AppHandle) -> Result<Vec<Profile>, String> {
    let store = app.store("profiles.json").map_err(|e| format!("store: {e}"))?;
    store
        .get(STORE_KEY)
        .and_then(|v| serde_json::from_value::<Vec<Profile>>(v.clone()).ok())
        .or_else(|| Some(vec![]))
        .ok_or("failed to deserialize profiles".into())
}

fn save_to_store(app: &AppHandle, profiles: &[Profile]) -> Result<(), String> {
    let store = app.store("profiles.json").map_err(|e| format!("store: {e}"))?;
    let value = serde_json::to_value(profiles).map_err(|e| format!("serialize: {e}"))?;
    store.set(STORE_KEY, value);
    store.save().map_err(|e| format!("save: {e}"))
}

#[tauri::command]
pub fn list_profiles(app: AppHandle) -> Result<Vec<Profile>, String> {
    load_from_store(&app)
}

#[tauri::command]
pub fn add_profile(app: AppHandle, label: String, host: String, port: u16) -> Result<Profile, String> {
    let profile = Profile::new(&label, &host, port);
    let mut profiles = load_from_store(&app)?;
    profiles.push(profile.clone());
    save_to_store(&app, &profiles)?;
    Ok(profile)
}

#[tauri::command]
pub fn update_profile(
    app: AppHandle,
    id: String,
    label: String,
    host: String,
    port: u16,
) -> Result<Profile, String> {
    let mut profiles = load_from_store(&app)?;
    let profile = profiles.iter_mut().find(|p| p.id == id).ok_or("profile not found")?;
    profile.label = label.trim().to_string();
    profile.host = host.trim().to_string();
    profile.port = port;
    let updated = profile.clone();
    save_to_store(&app, &profiles)?;
    Ok(updated)
}

#[tauri::command]
pub fn delete_profile(app: AppHandle, id: String) -> Result<(), String> {
    let mut profiles = load_from_store(&app)?;
    let before = profiles.len();
    profiles.retain(|p| p.id != id);
    if profiles.len() == before {
        return Err("profile not found".into());
    }
    save_to_store(&app, &profiles)
}

#[tauri::command]
pub fn activate_profile(
    app: AppHandle,
    id: String,
    state: State<'_, AppState>,
) -> Result<Profile, String> {
    let profiles = load_from_store(&app)?;
    let profile = profiles.into_iter().find(|p| p.id == id).ok_or("profile not found")?;
    let mut active = state.active_profile.lock().map_err(|e| e.to_string())?;
    *active = Some(profile.clone());
    Ok(profile)
}

#[tauri::command]
pub fn deactivate_profile(state: State<'_, AppState>) -> Result<(), String> {
    let mut active = state.active_profile.lock().map_err(|e| e.to_string())?;
    *active = None;
    Ok(())
}

#[tauri::command]
pub fn export_profiles(app: AppHandle) -> Result<String, String> {
    let profiles = load_from_store(&app)?;
    serde_json::to_string_pretty(&profiles).map_err(|e| format!("serialize: {e}"))
}

#[tauri::command]
pub fn import_profiles(app: AppHandle, json: String) -> Result<usize, String> {
    let imported: Vec<Profile> = serde_json::from_str(&json).map_err(|e| format!("invalid JSON: {e}"))?;
    let mut existing = load_from_store(&app)?;
    let count = imported.len();
    // Merge: skip duplicates by label
    for p in imported {
        if !existing.iter().any(|e| e.label.to_lowercase() == p.label.to_lowercase()) {
            existing.push(p);
        }
    }
    save_to_store(&app, &existing)?;
    Ok(count)
}
