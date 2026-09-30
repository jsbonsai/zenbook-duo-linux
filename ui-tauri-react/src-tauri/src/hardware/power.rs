use serde::{Deserialize, Serialize};
use std::{fs, path::Path, process::Command, sync::Mutex};

const PREFS: &str = "/var/lib/zenbook-duo/power-control.json";
const TURBO: &str = "/sys/devices/system/cpu/intel_pstate/no_turbo";
const CHARGE: &str = "/sys/class/power_supply/BAT0/charge_control_end_threshold";
static WRITE_LOCK: Mutex<()> = Mutex::new(());

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PowerPreferences {
    pub profile: Option<String>,
    pub turbo_enabled: Option<bool>,
    pub charge_limit: Option<u8>,
}
impl Default for PowerPreferences {
    fn default() -> Self {
        Self {
            profile: None,
            turbo_enabled: None,
            charge_limit: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum PowerAction {
    SetProfile { profile: String },
    SetTurbo { enabled: bool },
    SetChargeLimit { percent: u8 },
}
impl PowerAction {
    fn validate(&self) -> Result<(), String> {
        match self {
            Self::SetProfile { profile }
                if !matches!(profile.as_str(), "power-saver" | "balanced" | "performance") =>
            {
                Err("Unknown power profile".into())
            }
            Self::SetChargeLimit { percent } if !(60..=100).contains(percent) => {
                Err("Charge ceiling must be between 60 and 100 percent".into())
            }
            _ => Ok(()),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PowerStatus {
    pub profile: Option<String>,
    pub available_profiles: Vec<String>,
    pub turbo_enabled: Option<bool>,
    pub charge_limit: Option<u8>,
    pub battery_percent: Option<f64>,
    pub battery_status: Option<String>,
    pub battery_health: Option<f64>,
    pub battery_cycles: Option<u64>,
    pub energy_full_wh: Option<f64>,
    pub energy_design_wh: Option<f64>,
    pub cpu_temperature: Option<f64>,
    pub fan_rpm: Vec<u64>,
    pub preferences: PowerPreferences,
    pub persistence_error: Option<String>,
}
fn read(path: impl AsRef<Path>) -> Option<String> {
    fs::read_to_string(path).ok().map(|v| v.trim().to_owned())
}
fn number(path: impl AsRef<Path>) -> Option<f64> {
    read(path)?.parse().ok()
}
fn profile_command(args: &[&str]) -> Result<String, String> {
    let out = Command::new("/usr/bin/powerprofilesctl")
        .args(args)
        .output()
        .map_err(|e| format!("Power profiles unavailable: {e}"))?;
    if !out.status.success() {
        return Err(format!(
            "Power profile operation failed: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        ));
    }
    Ok(String::from_utf8_lossy(&out.stdout).trim().to_owned())
}
fn load_preferences() -> Result<PowerPreferences, String> {
    match fs::read_to_string(PREFS) {
        Ok(s) => serde_json::from_str(&s)
            .map_err(|e| format!("Cannot parse saved power preferences: {e}")),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(PowerPreferences::default()),
        Err(e) => Err(format!("Cannot read saved power preferences: {e}")),
    }
}
fn save_preferences(prefs: &PowerPreferences) -> Result<(), String> {
    use std::io::Write;
    use std::os::unix::fs::OpenOptionsExt;
    let temporary = format!("{PREFS}.tmp");
    let mut f = fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .mode(0o600)
        .open(&temporary)
        .map_err(|e| format!("Cannot save power preferences: {e}"))?;
    f.write_all(
        serde_json::to_string_pretty(prefs)
            .map_err(|e| e.to_string())?
            .as_bytes(),
    )
    .map_err(|e| e.to_string())?;
    f.sync_all().map_err(|e| e.to_string())?;
    fs::rename(&temporary, PREFS).map_err(|e| format!("Cannot commit power preferences: {e}"))
}
pub fn status() -> PowerStatus {
    let battery = Path::new("/sys/class/power_supply/BAT0");
    let full = number(battery.join("energy_full")).map(|n| n / 1_000_000.0);
    let design = number(battery.join("energy_full_design")).map(|n| n / 1_000_000.0);
    let mut temperature = None;
    let mut fan_rpm = Vec::new();
    if let Ok(entries) = fs::read_dir("/sys/class/hwmon") {
        for entry in entries.flatten() {
            let dir = entry.path();
            match read(dir.join("name")).as_deref() {
                Some("coretemp") => {
                    for i in 1..=32 {
                        if read(dir.join(format!("temp{i}_label")))
                            .is_some_and(|label| label.starts_with("Package id"))
                        {
                            temperature =
                                number(dir.join(format!("temp{i}_input"))).map(|n| n / 1000.0);
                        }
                    }
                }
                Some("asus") | Some("asus_hwmon") => {
                    for i in 1..=8 {
                        if let Some(n) = number(dir.join(format!("fan{i}_input"))) {
                            fan_rpm.push(n as u64);
                        }
                    }
                }
                _ => (),
            }
        }
    }
    let available_profiles = profile_command(&["list"])
        .unwrap_or_default()
        .lines()
        .map(|line| {
            line.trim()
                .trim_start_matches('*')
                .trim()
                .trim_end_matches(':')
        })
        .filter(|line| matches!(*line, "power-saver" | "balanced" | "performance"))
        .map(str::to_owned)
        .collect();
    let (preferences, persistence_error) = match load_preferences() {
        Ok(p) => (p, None),
        Err(e) => (PowerPreferences::default(), Some(e)),
    };
    PowerStatus {
        profile: profile_command(&["get"]).ok(),
        available_profiles,
        turbo_enabled: read(TURBO).and_then(|v| match v.as_str() {
            "0" => Some(true),
            "1" => Some(false),
            _ => None,
        }),
        charge_limit: read(CHARGE).and_then(|v| v.parse().ok()),
        battery_percent: number(battery.join("capacity")),
        battery_status: read(battery.join("status")),
        battery_health: full
            .zip(design)
            .filter(|(_, d)| *d > 0.0)
            .map(|(f, d)| f / d * 100.0),
        battery_cycles: read(battery.join("cycle_count")).and_then(|s| s.parse().ok()),
        energy_full_wh: full,
        energy_design_wh: design,
        cpu_temperature: temperature,
        fan_rpm,
        preferences,
        persistence_error,
    }
}
fn apply(action: &PowerAction) -> Result<(), String> {
    action.validate()?;
    match action {
        PowerAction::SetProfile { profile } => {
            profile_command(&["set", profile])?;
            if profile_command(&["get"])? != *profile {
                return Err("Requested profile was not applied".into());
            }
        }
        PowerAction::SetTurbo { enabled } => {
            let value = if *enabled { "0" } else { "1" };
            fs::write(TURBO, value).map_err(|e| format!("Cannot change turbo: {e}"))?;
            if read(TURBO).as_deref() != Some(value) {
                return Err("Turbo readback differs from request".into());
            }
        }
        PowerAction::SetChargeLimit { percent } => {
            fs::write(CHARGE, percent.to_string())
                .map_err(|e| format!("Cannot change charging ceiling: {e}"))?;
            if read(CHARGE).and_then(|s| s.parse::<u8>().ok()) != Some(*percent) {
                return Err("Charge ceiling readback differs from request".into());
            }
        }
    }
    Ok(())
}
pub fn set(action: PowerAction) -> Result<PowerStatus, String> {
    let _guard = WRITE_LOCK.lock().map_err(|e| e.to_string())?;
    action.validate()?;
    let mut prefs = load_preferences()?;
    apply(&action)?;
    match action {
        PowerAction::SetProfile { profile } => prefs.profile = Some(profile),
        PowerAction::SetTurbo { enabled } => prefs.turbo_enabled = Some(enabled),
        PowerAction::SetChargeLimit { percent } => prefs.charge_limit = Some(percent),
    }
    save_preferences(&prefs)
        .map_err(|e| format!("Hardware changed, but preference was not saved: {e}"))?;
    Ok(status())
}
pub fn restore() -> Result<(), String> {
    let _guard = WRITE_LOCK.lock().map_err(|e| e.to_string())?;
    let prefs = load_preferences()?;
    let mut errors = Vec::new();
    let mut actions = Vec::new();
    if let Some(profile) = prefs.profile {
        actions.push(PowerAction::SetProfile { profile });
    }
    if let Some(enabled) = prefs.turbo_enabled {
        actions.push(PowerAction::SetTurbo { enabled });
    }
    if let Some(percent) = prefs.charge_limit {
        actions.push(PowerAction::SetChargeLimit { percent });
    }
    for action in actions {
        if let Err(e) = apply(&action) {
            errors.push(e);
        }
    }
    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors.join("; "))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rejects_unrecognized_profile_and_out_of_range_ceiling() {
        for profile in ["", "quiet", "performance;touch /tmp/x", "../balanced"] {
            assert!(PowerAction::SetProfile {
                profile: profile.into()
            }
            .validate()
            .is_err());
        }
        for percent in [0, 59, 101, 255] {
            assert!(PowerAction::SetChargeLimit { percent }.validate().is_err());
        }
        for percent in [60, 70, 75, 80, 100] {
            assert!(PowerAction::SetChargeLimit { percent }.validate().is_ok());
        }
    }
    #[test]
    fn old_or_missing_preferences_do_not_enable_any_policy() {
        assert!(PowerPreferences::default().profile.is_none());
        assert!(PowerPreferences::default().turbo_enabled.is_none());
        assert!(PowerPreferences::default().charge_limit.is_none());
    }
}
