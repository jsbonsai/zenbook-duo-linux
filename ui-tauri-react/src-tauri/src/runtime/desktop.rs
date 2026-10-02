use crate::commands::desktop::{load_desktop_settings, qdbus, run, DesktopSettings};
use evdev::{Device, EventType};
use nix::fcntl::{fcntl, FcntlArg, OFlag};
use std::os::fd::AsRawFd;
use std::{
    collections::{HashMap, HashSet},
    fs,
    path::PathBuf,
    process::Command,
    time::{Duration, Instant},
};

pub struct DesktopService;
#[zbus::interface(name = "org.jsbonsai.ZenbookDuo.Desktop")]
impl DesktopService {
    fn action(&self, name: &str) -> zbus::fdo::Result<()> {
        action(name).map_err(zbus::fdo::Error::Failed)
    }
}
pub async fn connect() -> Result<zbus::Connection, String> {
    zbus::connection::Builder::session()
        .map_err(|e| e.to_string())?
        .name("org.jsbonsai.ZenbookDuo")
        .map_err(|e| e.to_string())?
        .serve_at("/Desktop", DesktopService)
        .map_err(|e| e.to_string())?
        .build()
        .await
        .map_err(|e| e.to_string())
}
fn osd(method: &str, args: &[&str]) -> Result<(), String> {
    if !load_desktop_settings().hud_enabled {
        return Ok(());
    }
    let mut all = vec!["org.kde.plasmashell", "/org/kde/osdService"];
    let name = format!("org.kde.osdService.{method}");
    all.push(&name);
    all.extend(args);
    qdbus(&all).map(|_| ())
}
fn hint(icon: &str, text: &str) -> Result<(), String> {
    osd("showText", &[icon, text])
}
fn executable(name: &str) -> Result<PathBuf, String> {
    let home = dirs::home_dir().unwrap_or_default();
    for dir in [
        home.join(".local/bin"),
        home.join(".cargo/bin"),
        PathBuf::from("/usr/local/bin"),
        PathBuf::from("/usr/bin"),
    ] {
        let p = dir.join(name);
        if p.is_file() {
            return Ok(p);
        }
    }
    Err(format!("{name} is not installed"))
}
fn spawn(program: PathBuf, args: &[&str]) -> Result<(), String> {
    let mut child = Command::new(program)
        .args(args)
        .spawn()
        .map_err(|e| e.to_string())?;
    std::thread::spawn(move || {
        let _ = child.wait();
    });
    Ok(())
}
pub fn action(name: &str) -> Result<(), String> {
    let settings = load_desktop_settings();
    let selected = match name {
        "f7" => settings.f7_action.as_str(),
        "f8" => settings.f8_action.as_str(),
        "f12" => settings.f12_action.as_str(),
        other => other,
    };
    match selected {
        "none" => Ok(()),
        "show_controls" => spawn(executable("zenbook-duo-control")?, &[]),
        "codex" => {
            let home = dirs::home_dir().ok_or("Home directory unavailable")?;
            let work = if home.join("_dev").is_dir() {
                home.join("_dev")
            } else {
                home
            };
            let codex = executable("codex")?;
            let term = executable(&settings.terminal)?;
            let work = work.to_string_lossy();
            let codex = codex.to_string_lossy();
            if settings.terminal == "konsole" {
                spawn(term, &["--workdir", &work, "-e", &codex])
            } else {
                spawn(term, &["--working-directory", &work, "-e", &codex])
            }
        }
        "emoji" => spawn(executable("plasma-emojier")?, &[]),
        "mic_mute" => {
            run("wpctl", &["set-mute", "@DEFAULT_AUDIO_SOURCE@", "toggle"])?;
            let muted = run("wpctl", &["get-volume", "@DEFAULT_AUDIO_SOURCE@"])?.contains("MUTED");
            hint(
                "audio-input-microphone",
                if muted {
                    "Microphone muted"
                } else {
                    "Microphone on"
                },
            )
        }
        "toggle_touchpad" => {
            let pads = duo_touchpads()?;
            if pads.is_empty() {
                return Err("No touchpad detected".into());
            }
            let enable = !property(&pads[0], "enabled")?.eq("true");
            for pad in pads {
                set_bool(&pad, "enabled", enable)?;
            }
            osd(
                "touchpadEnabledChanged",
                &[if enable { "true" } else { "false" }],
            )
        }
        "keyboard_brightness" => {
            let level = crate::hardware::sysfs::read_backlight_level();
            osd(
                "keyboardBrightnessChanged",
                &[&(u32::from(level) * 100 / 3).to_string()],
            )
        }
        "brightness" => {
            let max = crate::hardware::sysfs::read_max_brightness().max(1);
            osd(
                "brightnessChanged",
                &[&(crate::hardware::sysfs::read_display_brightness() * 100 / max).to_string()],
            )
        }
        "cycle_layout" | "swap_positions" => change_layout(selected),
        "swap_windows" => {
            let layout = crate::hardware::display_layout::get_display_layout()?;
            if !["eDP-1", "eDP-2"]
                .iter()
                .all(|c| layout.displays.iter().any(|d| &d.connector == c))
            {
                return Err("Enable both screens before swapping windows".into());
            }
            load_kwin_script("duo-window-swap", WINDOW_SWAP)?;
            hint("view-refresh", "Windows swapped between Duo screens")
        }
        _ => Err("Unknown desktop action".into()),
    }
}
fn change_layout(action: &str) -> Result<(), String> {
    let mut layout = crate::hardware::display_layout::get_display_layout()?;
    let a = layout
        .displays
        .iter()
        .position(|d| d.connector == "eDP-1")
        .ok_or("Upper screen unavailable")?;
    let b = layout
        .displays
        .iter()
        .position(|d| d.connector == "eDP-2")
        .ok_or("Enable the lower screen first")?;
    if action == "swap_positions" {
        let pos = (layout.displays[a].x, layout.displays[a].y);
        (layout.displays[a].x, layout.displays[a].y) = (layout.displays[b].x, layout.displays[b].y);
        (layout.displays[b].x, layout.displays[b].y) = pos;
    } else {
        let stacked = layout.displays[a].x == layout.displays[b].x;
        let width = (layout.displays[a].width as f64 / layout.displays[a].scale).round() as i32;
        let height = (layout.displays[a].height as f64 / layout.displays[a].scale).round() as i32;
        let pos = (layout.displays[a].x, layout.displays[a].y);
        layout.displays[b].x = pos.0 + if stacked { width } else { 0 };
        layout.displays[b].y = pos.1 + if stacked { 0 } else { height };
    }
    crate::hardware::display_layout::apply_display_layout(&layout)?;
    let mut preferences = crate::commands::settings::load_settings_local();
    preferences.saved_display_layout = Some(layout);
    crate::commands::settings::save_settings(preferences)?;
    hint("video-display", "Duo layout updated")
}
fn plasma(script: &str) -> Result<String, String> {
    qdbus(&[
        "org.kde.plasmashell",
        "/PlasmaShell",
        "org.kde.PlasmaShell.evaluateScript",
        script,
    ])
}
pub fn wallpaper_script(settings: &DesktopSettings) -> Result<String, String> {
    let lower = if settings.sync_wallpapers {
        &settings.upper_wallpaper
    } else {
        &settings.lower_wallpaper
    };
    let mut script = String::new();
    for (connector, path) in [("eDP-1", &settings.upper_wallpaper), ("eDP-2", lower)] {
        if path.is_empty() {
            continue;
        }
        let uri = tauri::Url::from_file_path(path)
            .map_err(|_| "Wallpaper must be an absolute local path")?;
        let uri = serde_json::to_string(uri.as_str()).map_err(|e| e.to_string())?;
        script += &format!("var screen=screenForConnector({connector:?}); if(screen>=0) {{ desktops().forEach(function(d) {{ if(d.screen===screen) {{ d.wallpaperPlugin='org.kde.image'; d.currentConfigGroup=['Wallpaper','org.kde.image','General']; d.writeConfig('Image',{uri}); }} }}); }}\n");
    }
    Ok(script)
}
pub fn apply_wallpapers(settings: &DesktopSettings) -> Result<(), String> {
    let script = wallpaper_script(settings)?;
    if script.is_empty() {
        return Ok(());
    }
    let output = plasma(&script)?;
    if output.contains("Error") {
        return Err(output);
    }
    Ok(())
}
fn property(path: &str, property: &str) -> Result<String, String> {
    qdbus(&[
        "org.kde.KWin",
        path,
        "org.freedesktop.DBus.Properties.Get",
        "org.kde.KWin.InputDevice",
        property,
    ])
}
fn set_bool(path: &str, name: &str, value: bool) -> Result<(), String> {
    run(
        "busctl",
        &[
            "--user",
            "set-property",
            "org.kde.KWin",
            path,
            "org.kde.KWin.InputDevice",
            name,
            "b",
            if value { "true" } else { "false" },
        ],
    )
    .map(|_| ())
}
fn input_devices(kind: &str) -> Result<Vec<String>, String> {
    Ok(qdbus(&["org.kde.KWin"])?
        .lines()
        .filter(|p| p.contains("/InputDevice/event") && property(p, kind).as_deref() == Ok("true"))
        .map(str::to_owned)
        .collect())
}
fn duo_touchpads() -> Result<Vec<String>, String> {
    Ok(input_devices("touchpad")?
        .into_iter()
        .filter(|p| {
            property(p, "name")
                .map(|n| n.contains("Zenbook Duo Keyboard") || n.contains("ASUS_DUO"))
                .unwrap_or(false)
        })
        .collect())
}
fn map_touchscreens() -> Result<(), String> {
    for p in input_devices("touch")? {
        let name = property(&p, "name")?;
        let output = if name.starts_with("ELAN9008") {
            "eDP-1"
        } else if name.starts_with("ELAN9009") {
            "eDP-2"
        } else {
            continue;
        };
        if property(&p, "outputName")? != output {
            run(
                "busctl",
                &[
                    "--user",
                    "set-property",
                    "org.kde.KWin",
                    &p,
                    "org.kde.KWin.InputDevice",
                    "outputName",
                    "s",
                    output,
                ],
            )?;
        }
    }
    Ok(())
}
const WINDOW_SWAP: &str = r#"
var a=workspace.screens.find(function(s){return s.name==='eDP-1';});
var b=workspace.screens.find(function(s){return s.name==='eDP-2';});
if(a && b) {
 var moves=workspace.windowList().filter(function(w){return w.normalWindow && (w.output===a || w.output===b);}).map(function(w){return [w,w.output===a?b:a];});
 moves.forEach(function(m){workspace.sendClientToScreen(m[0],m[1]);});
}
"#;
const SHORTCUTS: &str = r#"
function duo(action){callDBus('org.jsbonsai.ZenbookDuo','/Desktop','org.jsbonsai.ZenbookDuo.Desktop','Action',action);}
registerShortcut('DuoControl','Open Zenbook Duo Control','Meta+Shift+D',function(){duo('show_controls');});
registerShortcut('DuoCodex','Open Codex terminal','Meta+Shift+C',function(){duo('codex');});
registerShortcut('DuoSwap','Swap Duo windows','Meta+Shift+S',function(){duo('swap_windows');});
"#;
fn load_kwin_script(name: &str, content: &str) -> Result<(), String> {
    let dir = dirs::data_local_dir()
        .ok_or("Data directory unavailable")?
        .join("zenbook-duo/scripts");
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let path = dir.join(format!("{name}.js"));
    fs::write(&path, content).map_err(|e| e.to_string())?;
    let _ = qdbus(&[
        "org.kde.KWin",
        "/Scripting",
        "org.kde.kwin.Scripting.unloadScript",
        name,
    ]);
    let id = qdbus(&[
        "org.kde.KWin",
        "/Scripting",
        "org.kde.kwin.Scripting.loadScript",
        &path.to_string_lossy(),
        name,
    ])?;
    if id.parse::<i32>().unwrap_or(-1) < 0 {
        return Err("KWin could not load the desktop shortcut script".into());
    }
    qdbus(&["org.kde.KWin", "/Scripting", "org.kde.kwin.Scripting.start"])?;
    Ok(())
}

/// Only actual text/editing keys extend the guard: media keys and modifiers do not.
fn typing_key(code: u16, value: i32) -> bool {
    matches!(value, 1 | 2)
        && matches!(code, 2..=13 | 14..=28 | 30..=41 | 43..=53 | 57 | 71..=83 | 96 | 111)
}
fn open_keyboards() -> HashMap<PathBuf, Device> {
    let mut devices = HashMap::new();
    if let Ok(entries) = fs::read_dir("/dev/input") {
        for entry in entries.flatten() {
            let path = entry.path();
            if !entry.file_name().to_string_lossy().starts_with("event") {
                continue;
            }
            if let Ok(device) = Device::open(&path) {
                let name = device.name().unwrap_or("");
                if !(name.contains("Zenbook Duo") || name.contains("ASUS_DUO"))
                    || name.contains("Touchpad")
                    || device
                        .supported_keys()
                        .map(|s| !s.contains(evdev::Key::KEY_A))
                        .unwrap_or(true)
                {
                    continue;
                }
                let flags = fcntl(device.as_raw_fd(), FcntlArg::F_GETFL).unwrap_or(0);
                if fcntl(
                    device.as_raw_fd(),
                    FcntlArg::F_SETFL(OFlag::from_bits_truncate(flags) | OFlag::O_NONBLOCK),
                )
                .is_ok()
                {
                    devices.insert(path, device);
                }
            }
        }
    }
    devices
}
fn recovery_path() -> PathBuf {
    crate::commands::desktop::preferences_path().with_file_name("typing-guard-recovery.json")
}
#[derive(Clone)]
struct Touchpad {
    path: String,
    name: String,
}
fn restore_taps(saved: &mut HashMap<String, bool>, pads: &[Touchpad]) {
    for pad in pads {
        if let Some(value) = saved.get(&pad.name).copied() {
            if set_bool(&pad.path, "tapToClick", value).is_ok() {
                saved.remove(&pad.name);
            }
        }
    }
    persist_guard(saved);
}
fn persist_guard(saved: &HashMap<String, bool>) {
    let path = recovery_path();
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    if saved.is_empty() {
        let _ = fs::remove_file(path);
    } else if let Ok(json) = serde_json::to_vec(saved) {
        let _ = fs::write(path, json);
    }
}
fn guard_active(enabled: bool, elapsed: Option<Duration>, delay_ms: u64) -> bool {
    enabled
        && elapsed
            .map(|d| d < Duration::from_millis(delay_ms))
            .unwrap_or(false)
}
pub fn start() {
    let shared = std::sync::Arc::new(std::sync::Mutex::new((
        load_desktop_settings(),
        Vec::<Touchpad>::new(),
    )));
    let maintenance = shared.clone();
    std::thread::spawn(move || {
        if let Err(e) = load_kwin_script("duo-shortcuts", SHORTCUTS) {
            log::warn!("Duo shortcuts: {e}");
        }
        let mut last_inputs = String::new();
        let mut pads = Vec::new();
        let mut last_wallpaper_key = String::new();
        loop {
            let preferences = load_desktop_settings();
            if let Ok(inputs) = qdbus(&["org.kde.KWin"]) {
                if inputs != last_inputs {
                    pads = duo_touchpads()
                        .unwrap_or_default()
                        .into_iter()
                        .filter_map(|path| {
                            property(&path, "name")
                                .ok()
                                .map(|name| Touchpad { path, name })
                        })
                        .collect();
                    let _ = map_touchscreens();
                    last_inputs = inputs;
                }
            }
            if !preferences.upper_wallpaper.is_empty() || !preferences.lower_wallpaper.is_empty() {
                if let Ok(outputs)=plasma("print(screenForConnector('eDP-1')+','+screenForConnector('eDP-2')+':'+currentActivity());") {
                    let key=format!("{outputs}:{:?}:{:?}:{}",preferences.upper_wallpaper,preferences.lower_wallpaper,preferences.sync_wallpapers);
                    if key!=last_wallpaper_key {
                        match apply_wallpapers(&preferences) { Ok(())=>last_wallpaper_key=key, Err(e)=>log::warn!("Duo wallpaper: {e}") }
                    }
                }
            } else {
                last_wallpaper_key.clear();
            }
            if let Ok(mut state) = maintenance.lock() {
                *state = (preferences, pads.clone());
            }
            std::thread::sleep(Duration::from_secs(3));
        }
    });
    std::thread::spawn(move || {
        let mut keyboards = open_keyboards();
        let mut saved: HashMap<String, bool> = fs::read(recovery_path())
            .ok()
            .and_then(|s| serde_json::from_slice(&s).ok())
            .unwrap_or_default();
        let mut last_scan = Instant::now();
        let mut last_typing = None;
        let mut attempted = HashSet::new();
        loop {
            if last_scan.elapsed() >= Duration::from_secs(2) {
                for (p, d) in open_keyboards() {
                    keyboards.entry(p).or_insert(d);
                }
                last_scan = Instant::now();
            }
            let mut lost = HashSet::new();
            for (path, device) in &mut keyboards {
                match device.fetch_events() {
                    Ok(events) => {
                        for event in events {
                            if event.event_type() == EventType::KEY
                                && typing_key(event.code(), event.value())
                            {
                                last_typing = Some(Instant::now());
                            }
                        }
                    }
                    Err(e)
                        if e.kind() == std::io::ErrorKind::WouldBlock
                            || e.kind() == std::io::ErrorKind::Interrupted => {}
                    Err(_) => {
                        lost.insert(path.clone());
                    }
                }
            }
            keyboards.retain(|p, _| !lost.contains(p));
            let (preferences, pads) = match shared.lock() {
                Ok(s) => s.clone(),
                Err(_) => break,
            };
            let active = guard_active(
                preferences.typing_guard,
                last_typing.map(|t| t.elapsed()),
                preferences.typing_delay_ms,
            );
            if active {
                for pad in &pads {
                    if saved.contains_key(&pad.name) {
                        continue;
                    }
                    if attempted.insert(pad.name.clone())
                        && property(&pad.path, "tapToClick").as_deref() == Ok("true")
                    {
                        saved.insert(pad.name.clone(), true);
                        persist_guard(&saved);
                        if set_bool(&pad.path, "tapToClick", false).is_err() {
                            saved.remove(&pad.name);
                            persist_guard(&saved);
                        }
                    }
                }
            } else {
                attempted.clear();
                if !saved.is_empty() {
                    restore_taps(&mut saved, &pads);
                }
            }
            std::thread::sleep(Duration::from_millis(25));
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn guard_restores_taps_after_idle_or_disable() {
        assert!(guard_active(true, Some(Duration::from_millis(100)), 750));
        assert!(!guard_active(true, Some(Duration::from_millis(750)), 750));
        assert!(!guard_active(false, Some(Duration::ZERO), 750));
        assert!(!guard_active(true, None, 750));
    }
    #[test]
    fn only_typing_keys_extend_guard() {
        assert!(typing_key(30, 1));
        assert!(typing_key(30, 2));
        assert!(!typing_key(30, 0));
        assert!(!typing_key(42, 1));
        assert!(!typing_key(59, 1));
        assert!(!typing_key(114, 1));
    }
    #[test]
    fn wallpaper_paths_cannot_inject_javascript() {
        let mut s = DesktopSettings::default();
        s.upper_wallpaper = "/tmp/x'\";evil();.png".into();
        let script = wallpaper_script(&s).unwrap();
        assert!(script.contains("screenForConnector(\"eDP-1\")"));
        assert!(script.contains("file:///tmp/"));
        assert!(!script.contains("writeConfig('Image','file"));
    }
    #[test]
    fn sync_wallpaper_keeps_connector_identity() {
        let mut s = DesktopSettings::default();
        s.upper_wallpaper = "/tmp/upper.png".into();
        s.lower_wallpaper = "/tmp/lower.png".into();
        s.sync_wallpapers = true;
        let script = wallpaper_script(&s).unwrap();
        assert!(!script.contains("lower.png"));
        assert!(script.contains("eDP-2"));
    }
}
