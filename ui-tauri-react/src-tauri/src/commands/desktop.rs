//! Desktop integration runs as the logged-in user, never in the privileged daemon.
use serde::{Deserialize, Serialize};
use std::{fs, path::PathBuf, process::Command};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct DesktopSettings {
    pub keep_dual_on_usb: bool,
    pub typing_guard: bool,
    pub typing_delay_ms: u64,
    pub hud_enabled: bool,
    pub terminal: String,
    pub f7_action: String,
    pub f8_action: String,
    pub f12_action: String,
    pub upper_wallpaper: String,
    pub lower_wallpaper: String,
    pub sync_wallpapers: bool,
}
impl Default for DesktopSettings {
    fn default() -> Self {
        Self {
            keep_dual_on_usb: false,
            typing_guard: true,
            typing_delay_ms: 750,
            hud_enabled: true,
            terminal: "konsole".into(),
            f7_action: "cycle_layout".into(),
            f8_action: "swap_windows".into(),
            f12_action: "codex".into(),
            upper_wallpaper: String::new(),
            lower_wallpaper: String::new(),
            sync_wallpapers: false,
        }
    }
}
pub fn preferences_path() -> PathBuf {
    crate::commands::settings::config_base_dir().join("zenbook-duo/desktop.json")
}
#[tauri::command]
pub fn load_desktop_settings() -> DesktopSettings {
    fs::read(preferences_path())
        .ok()
        .and_then(|s| serde_json::from_slice(&s).ok())
        .unwrap_or_default()
}
pub fn run(program: &str, args: &[&str]) -> Result<String, String> {
    let out = Command::new(program)
        .args(args)
        .output()
        .map_err(|e| format!("{program}: {e}"))?;
    if !out.status.success() {
        return Err(format!(
            "{program}: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        ));
    }
    Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
}
pub fn qdbus(args: &[&str]) -> Result<String, String> {
    run("qdbus6", args)
}
pub fn allowed_action(action: &str) -> bool {
    matches!(
        action,
        "none"
            | "show_controls"
            | "codex"
            | "emoji"
            | "cycle_layout"
            | "swap_windows"
            | "swap_positions"
            | "mic_mute"
            | "toggle_touchpad"
            | "keyboard_brightness"
    )
}
fn validate(settings: &DesktopSettings) -> Result<(), String> {
    if !(200..=2000).contains(&settings.typing_delay_ms) {
        return Err("Typing delay must be 200–2000 ms".into());
    }
    if !matches!(settings.terminal.as_str(), "konsole" | "alacritty") {
        return Err("Choose Konsole or Alacritty".into());
    }
    for action in [
        &settings.f7_action,
        &settings.f8_action,
        &settings.f12_action,
    ] {
        if !allowed_action(action) {
            return Err("Unknown keyboard action".into());
        }
    }
    for image in [&settings.upper_wallpaper, &settings.lower_wallpaper] {
        if !image.is_empty() {
            validate_image(image)?;
        }
    }
    Ok(())
}
fn validate_image(path: &str) -> Result<PathBuf, String> {
    let path = fs::canonicalize(path).map_err(|e| format!("Wallpaper unavailable: {e}"))?;
    let ext = path
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    if !matches!(
        ext.as_str(),
        "png" | "jpg" | "jpeg" | "webp" | "avif" | "bmp"
    ) || !path.is_file()
    {
        return Err("Choose a PNG, JPEG, WebP, AVIF or BMP image".into());
    }
    if fs::metadata(&path).map_err(|e| e.to_string())?.len() > 32 * 1024 * 1024 {
        return Err("Wallpaper preview supports images up to 32 MB".into());
    }
    Ok(path)
}
#[tauri::command]
pub fn save_desktop_settings(settings: DesktopSettings) -> Result<(), String> {
    validate(&settings)?;
    // Apply before persisting so failed desktop operations are visible to the user.
    crate::runtime::desktop::apply_wallpapers(&settings)?;
    let path = preferences_path();
    fs::create_dir_all(path.parent().unwrap()).map_err(|e| e.to_string())?;
    let temp = path.with_extension("json.tmp");
    fs::write(
        &temp,
        serde_json::to_vec_pretty(&settings).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    fs::rename(temp, path).map_err(|e| e.to_string())
}
#[tauri::command]
pub fn choose_desktop_wallpaper() -> Result<Option<String>, String> {
    let out = Command::new("kdialog")
        .args([
            "--getopenfilename",
            &dirs::home_dir().unwrap_or_default().to_string_lossy(),
            "Images (*.png *.jpg *.jpeg *.webp *.avif *.bmp)",
            "--title",
            "Choose Duo wallpaper",
        ])
        .output()
        .map_err(|e| e.to_string())?;
    if !out.status.success() {
        return Ok(None);
    }
    let path = String::from_utf8_lossy(&out.stdout).trim().to_string();
    Ok(Some(validate_image(&path)?.to_string_lossy().into_owned()))
}
#[tauri::command]
pub fn desktop_wallpaper_preview(path: String) -> Result<Vec<u8>, String> {
    fs::read(validate_image(&path)?).map_err(|e| e.to_string())
}
#[tauri::command]
pub fn desktop_action(action: String) -> Result<(), String> {
    if !allowed_action(&action) {
        return Err("Unknown desktop action".into());
    }
    crate::runtime::desktop::action(&action)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rejects_commands_and_invalid_delay() {
        let mut s = DesktopSettings::default();
        s.f12_action = "sh -c evil".into();
        assert!(validate(&s).is_err());
        s = DesktopSettings::default();
        s.typing_delay_ms = 0;
        assert!(validate(&s).is_err());
        s = DesktopSettings::default();
        s.terminal = "bash".into();
        assert!(validate(&s).is_err());
    }
    #[test]
    fn old_preferences_receive_new_defaults() {
        let s: DesktopSettings = serde_json::from_str("{}").unwrap();
        assert!(s.typing_guard);
        assert_eq!(s.typing_delay_ms, 750);
    }
}
