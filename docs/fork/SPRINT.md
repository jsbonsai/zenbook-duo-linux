# Duo desktop sprint

Owner: jsbonsai. Upstream: zakstam/zenbook-duo-linux. Initial target: KDE Plasma Wayland on ASUS UX8406CA. Preserve existing display, docking, Bluetooth, touch and power behavior.

## Baseline already implemented

- KDE connector-specific mode IDs, Hz/mHz handling and rotation fixes.
- Power & battery controls: firmware profiles via power-profiles-daemon, turbo, charge ceiling, battery health/cycles, temperature and fan readings. Root operations are validated and saved preferences reapplied by the runtime.
- Lower dashboard redraw/polling cost, pausing power polling while inactive.
- USB remapper disconnect/startup repair, with regression tests. Native deployment/physical validation is distinct from a successful build.

## Sprint deliverables

1. **Keyboard actions and feedback.** Capture F7–F12 key events in USB and Bluetooth modes; distinguish actual key events from printed legends. Retain F1–F6 media/backlight/brightness and F11 emoji. Add microphone mute and app-launch actions only after identifying their events. Use native Plasma OSD for volume/mute, microphone and screen/keyboard brightness; fill gaps with a small text OSD. Show confirmed state after a successful action and avoid duplicate desktop OSD. Keep desktop D-Bus and application launching in the logged-in session agent; root handles hardware only.
2. **Open controls shortcut.** Add a configurable desktop shortcut to show/focus the existing application instance. Proposed default Meta+Shift+D, subject to conflict check. Launching from the tray and desktop entry remains available.
3. **F12 Codex launcher.** Configurable action, default Konsole running Codex from the user's development directory. Discover executable paths rather than hardcoding this machine's username. Use argument vectors, not shell command strings. Alacritty is an alternate when installed; show a clear message if Codex/terminal is missing. Desktop launching never runs as root.
4. **F8 display action.** Define whether this swaps windows, swaps desktop positions, or cycles layout presets. These are different operations; user selection is pending. Keep touch mapping aligned with physical panels and handle docking, external displays and absent lower panel.
5. **Independent wallpapers.** A page in the app to choose an image for each built-in panel, with previews and a sync option. Apply through Plasma's desktop containment API in the user session. Identify outputs by stable connector/UUID, never by current output index. Store per-panel choices and reconcile after dock/detach and login; preserve unrelated external-monitor wallpapers. Use a file picker and local image files for the first iteration. No root access is needed.
6. **Native installation and restart reliability.** Include brightness authorization and persistent KWin touchscreen assignment in installation, remove live-session assumptions, and make installation/reinstallation safe. Reuse desktop services and avoid a second competing media-key listener.

## Acceptance checks

- Keyboard events/action table verified in USB and Bluetooth modes, including Fn-lock behavior.
- One HUD indication per action; actual mute/brightness/keyboard-light state agrees with the indication.
- F12 opens one requested Codex terminal as the desktop user; application shortcut focuses existing controls.
- Wallpaper choices survive login and keyboard detach/redock; lower touch stays on lower panel.
- No remapper failure notification on normal USB removal; reconnect restores media keys.
- Saved power policy survives service restart; reboot/resume verified separately with user present.
- Rust library tests, frontend build and release build pass. Review frontend dependency audit findings separately from feature implementation; avoid blind lockfile upgrades.

## Remaining decisions and evidence

v0.4.0 implements the app controls, desktop actions, typing guard and installation path. User reported the live typing-guard preview works well. The remaining acceptance checks above require post-install physical validation.

F7 and F12 legends/events are not yet established for this physical keyboard. ASUS documents that hotkey functions vary by model; consult the UX8406CA manual and capture the device events before mapping. F8 defaults to swapping windows; a selector also offers swapping desktop positions or cycling stacked/side-by-side layouts. Custom HUD styling can follow native OSD validation if needed.

References: https://www.asus.com/us/supportonly/ux8406ca/helpdesk_manual/ and https://www.asus.com/us/support/faq/1044480/ .

### Feedback compatibility

The session agent observes confirmed speaker/microphone and primary screen/keyboard brightness changes every 350 ms and forwards changes to Plasma OSD. This covers firmware and legacy handlers that bypass KDE shortcuts, including changes from desktop sliders. It seeds initial state without displaying startup banners and honors the extra feedback toggle. Native and forwarded feedback reuse the same Plasma widget.

### October 4 responsiveness and charging fixes

USB cable charging and physical docking expose the same keyboard ID and USB port on this machine. The Desktop & keyboard page now offers “Keep both screens on when using USB”; this overrides dock display suppression without disabling USB remapping. It also keeps both panels on when physically docked. Existing installs default to previous behavior; the development machine has the override enabled at the user's request.

Idle audio feedback uses one persistent pactl subscription rather than two wpctl launches every 350 ms. Screen/keyboard brightness still reads sysfs without subprocesses. Background status polling no longer duplicates root compositor queries; user display liveness queries run every five seconds instead of every second. Static status dots replace continuous GPU animations. Desktop display updates can take up to five seconds to appear in background status. libpulse supplies pactl on Arch.

Validation: 145 preexisting Rust library tests passed, new charging override test passed, TypeScript/Vite frontend built. Wireless input sample: 133 event batches, p95 kernel-to-reader delivery 1.41 ms, max 4.70 ms; this does not measure over-the-air latency. GPU attribution needs privileged fdinfo access and is not yet complete. No recent GPU reset/oom log entries were observed. Root daemon installation remains necessary to apply its reduced display polling.

Battery evidence: September 30 saved health 91.5379%; October 4 full 67.697 Wh / design 75.076 Wh = 90.1713%, cycle count 28, charge ceiling 75%. These are firmware estimates and establish neither initial-new capacity nor the amount of physical degradation during ownership.

Privileged GPU follow-up: KWin render engine ~54.6%, System Monitor ~8.0%, Brave ~0.2% during one 5.64-second sample. This attributes activity during the sample, not the cause of all intermittent lag. Temporarily unloaded KDE blur for comparison; no persisted desktop-effect preference was changed. Session-agent group CPU after event-based audio changed from ~33% to ~27.6% of one core while the old system daemon continued requesting layouts every second. Root v0.3.7 still needs replacement to apply the five-second liveness polling. The user service is active again and temporary masks were removed.
