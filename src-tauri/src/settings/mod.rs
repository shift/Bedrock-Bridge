use std::path::PathBuf;

/// Whether autostart is supported on this platform.
/// Only Linux/macOS/Windows have a config dir for autostart entries.
pub fn is_supported() -> bool {
    dirs::config_dir().is_some()
}

#[tauri::command]
pub fn is_autostart_supported() -> bool {
    is_supported()
}

fn autostart_dir() -> Result<PathBuf, String> {
    let config = dirs::config_dir().ok_or("autostart not supported on this platform")?;
    Ok(config.join("autostart"))
}

fn autostart_file() -> Result<PathBuf, String> {
    Ok(autostart_dir()?.join("bedrock-bridge.desktop"))
}

const DESKTOP_ENTRY: &str = "[Desktop Entry]\n\
Type=Application\n\
Name=Bedrock Bridge\n\
Comment=UDP relay for Minecraft Bedrock Edition\n\
Exec=bedrock-bridge\n\
Icon=bedrock-bridge\n\
Terminal=false\n\
Categories=Network;Game;\n\
StartupNotify=false\n\
X-GNOME-Autostart-enabled=true\n";

#[tauri::command]
pub fn set_autostart(enable: bool) -> Result<(), String> {
    let path = autostart_file()?;
    if enable {
        let dir = autostart_dir()?;
        std::fs::create_dir_all(&dir).map_err(|e| format!("create autostart dir: {e}"))?;
        std::fs::write(&path, DESKTOP_ENTRY).map_err(|e| format!("write autostart file: {e}"))?;
        tracing::info!("Autostart enabled: {}", path.display());
    } else {
        if path.exists() {
            std::fs::remove_file(&path).map_err(|e| format!("remove autostart file: {e}"))?;
        }
        tracing::info!("Autostart disabled");
    }
    Ok(())
}

#[tauri::command]
pub fn is_autostart_enabled() -> Result<bool, String> {
    let path = autostart_file()?;
    Ok(path.exists())
}
